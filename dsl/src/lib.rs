use std::collections::{BTreeMap, BTreeSet};

use crabflow_core::{SequenceDesc, WorkflowDesc};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Attribute, Error, Expr, ExprAssign, Ident, Item, ItemFn, Meta, Path, Result, Stmt, Token,
};

#[proc_macro_attribute]
pub fn workflow(attr: TokenStream, input: TokenStream) -> TokenStream {
    let workflow = parse_macro_input!(input as Workflow);
    let params = parse_macro_input!(attr as WorkflowParams);
    let workflow = workflow.apply(params);
    let funs = workflow.tasks.iter().map(|task| &task.fun);
    let enum_values = workflow.tasks.iter().map(|task| {
        let id = &task.id;
        let about = format!("Run task `{id}`");
        quote! {
            #[command(about = #about)]
            #id
        }
    });
    let run_cases = workflow.tasks.iter().map(|task| {
        let id = &task.id;
        let fun_name = &task.fun.sig.ident;
        quote! {
            Self::#id => #fun_name().await
        }
    });
    let mut deps = BTreeMap::new();
    let tasks: BTreeMap<&Ident, &Task> =
        workflow.tasks.iter().map(|task| (&task.id, task)).collect();
    for task in tasks.values() {
        let task_deps = match all_dependencies(task, &tasks, &mut Default::default()) {
            Ok(deps) => deps,
            Err(err) => {
                return err.into_compile_error().into();
            }
        };
        deps.insert(&task.id, task_deps);
    }
    let workflow = WorkflowDesc {
        id: workflow.id.to_string(),
        seq: sequence(&deps, Default::default()).unwrap_or_default(),
    };
    let json = match serde_json::to_string(&workflow) {
        Ok(json) => json,
        Err(err) => {
            let err = Error::new(Span::call_site(), err.to_string());
            return err.into_compile_error().into();
        }
    };
    quote! {
        use crabflow::{
            clap as clap,
            common as crabflow_common,
            tokio as tokio,
        };

        const WORKFLOW_JSON: &str = #json;

        #[tokio::main]
        async fn main() -> crabflow::Result<()> {
            use clap::Parser;
            use crabflow_internal::*;

            let args = Args::parse();
            args.run().await?;
            Ok(())
        }

        #(#funs)*

        mod crabflow_internal {
            use clap::{self, Parser, Subcommand};
            use crabflow::{Result, load_workflow,};
            use crabflow_common::clap::DatabaseOptions;

            use super::*;

            #[derive(Clone, Debug, Eq, Parser, PartialEq)]
            #[command(author)]
            pub struct Args {
                #[command(subcommand)]
                pub cmd: Command,
                #[command(flatten)]
                pub opts: DatabaseOptions,
            }

            impl Args {
                pub async fn run(self) -> Result {
                    self.cmd.run(self.opts).await
                }
            }

            #[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
            pub enum Command {
                #[command(about = "Print JSON-encoded workflow")]
                Json,
                #[command(about = "Load workflow")]
                Load,
                #[command(subcommand, about = "Run task")]
                Run(Task),
            }

            impl Command {
                pub async fn run(self, opts: DatabaseOptions) -> Result {
                    match self {
                        Self::Json => {
                            println!("{WORKFLOW_JSON}");
                            Ok(())
                        },
                        Self::Load => load_workflow(WORKFLOW_JSON, opts).await,
                        Self::Run(task) => task.run().await,
                    }
                }
            }

            #[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
            #[allow(non_camel_case_types)]
            pub enum Task {
                #(#enum_values),*
            }

            impl Task {
                pub async fn run(self) -> Result {
                    match self {
                        #(#run_cases),*
                    }
                }
            }
        }
    }
    .into()
}

struct Task {
    deps: Vec<Ident>,
    fun: ItemFn,
    id: Ident,
    span: Span,
}

impl Task {
    fn apply(self, params: TaskParams) -> Self {
        Self {
            deps: params.deps.unwrap_or(self.deps),
            id: params.id.unwrap_or(self.id),
            ..self
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TaskParams {
    deps: Option<Vec<Ident>>,
    id: Option<Ident>,
}

impl Parse for TaskParams {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut span_start = input.span();
        let params = Punctuated::<ExprAssign, Token![,]>::parse_terminated(input)?;
        let mut id = None;
        let mut deps = None;
        for pair in params.into_pairs() {
            let (param, comma) = pair.into_tuple();
            let span = if let Some(comma) = comma {
                let span = span_start.join(comma.span).unwrap_or(span_start);
                span_start = comma.span;
                span
            } else {
                span_start
            };
            let key = ident_from_expr(*param.left)
                .ok_or_else(|| Error::new(span, "parameter should be [id]"))?;
            if key == "id" {
                let value = ident_from_expr(*param.right)
                    .ok_or_else(|| Error::new(span, "id should be ident"))?;
                id = Some(value);
            } else if key == "depends_on" {
                let value = depends_on(*param.right).ok_or_else(|| {
                    Error::new(
                        span,
                        "depends_on should be ident or array of idents (ex: [a, b])",
                    )
                })?;
                deps = Some(value);
            } else {
                let msg = format!("unknown parameter `{key}`");
                return Err(Error::new(span, msg));
            }
        }
        Ok(Self { deps, id })
    }
}

struct Workflow {
    id: Ident,
    tasks: Vec<Task>,
}

impl Workflow {
    fn apply(self, params: WorkflowParams) -> Self {
        Self {
            id: params.id.unwrap_or(self.id),
            tasks: self.tasks,
        }
    }
}

impl Parse for Workflow {
    fn parse(input: ParseStream) -> Result<Self> {
        let fun = ItemFn::parse(input)?;
        let mut tasks = vec![];
        for stmt in fun.block.stmts {
            if let Stmt::Item(Item::Fn(mut fun)) = stmt {
                if let Some(attr) = get_attr("task", &mut fun) {
                    let id = fun.sig.ident.clone();
                    let task = Task {
                        deps: Default::default(),
                        fun,
                        id,
                        span: attr.pound_token.span,
                    };
                    let params = if let Meta::List(meta) = attr.meta {
                        meta.parse_args_with(TaskParams::parse)?
                    } else {
                        Default::default()
                    };
                    tasks.push(task.apply(params));
                }
            }
        }
        Ok(Self {
            id: fun.sig.ident,
            tasks,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct WorkflowParams {
    id: Option<Ident>,
}

impl Parse for WorkflowParams {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut span_start = input.span();
        let params = Punctuated::<ExprAssign, Token![,]>::parse_terminated(input)?;
        let mut id = None;
        for pair in params.into_pairs() {
            let (param, comma) = pair.into_tuple();
            let span = if let Some(comma) = comma {
                let span = span_start.join(comma.span).unwrap_or(span_start);
                span_start = comma.span;
                span
            } else {
                span_start
            };
            let key = ident_from_expr(*param.left)
                .ok_or_else(|| Error::new(span, "parameter should be ident"))?;
            if key == "id" {
                let value = ident_from_expr(*param.right)
                    .ok_or_else(|| Error::new(span, "id should be ident"))?;
                id = Some(value);
            } else {
                let msg = format!("unknown parameter `{key}`");
                return Err(Error::new(span, msg));
            }
        }
        Ok(Self { id })
    }
}

fn all_dependencies<'a>(
    task: &'a Task,
    tasks: &'a BTreeMap<&Ident, &Task>,
    parents: &mut BTreeSet<&'a Ident>,
) -> Result<BTreeSet<&'a Ident>> {
    if parents.contains(&task.id) {
        Err(Error::new(task.span, "circular dependency"))
    } else {
        parents.insert(&task.id);
        let mut deps = BTreeSet::new();
        for id in &task.deps {
            deps.insert(id);
            let dep = tasks
                .get(id)
                .ok_or_else(|| Error::new(task.span, format!("task `{id}` doesn't exist")))?;
            let mut dep_deps = all_dependencies(dep, tasks, parents)?;
            deps.append(&mut dep_deps);
        }
        Ok(deps)
    }
}

fn get_attr(key: &str, fun: &mut ItemFn) -> Option<Attribute> {
    fun.attrs
        .iter()
        .position(|attr| attr.path().is_ident(key))
        .map(|idx| fun.attrs.remove(idx))
}

fn depends_on(expr: Expr) -> Option<Vec<Ident>> {
    match expr {
        Expr::Array(expr) => expr
            .elems
            .into_iter()
            .map(ident_from_expr)
            .collect::<Option<Vec<Ident>>>(),
        Expr::Path(expr) => {
            let id = ident_from_path(expr.path)?;
            Some(vec![id])
        }
        _ => None,
    }
}

fn ident_from_expr(expr: Expr) -> Option<Ident> {
    if let Expr::Path(expr) = expr {
        ident_from_path(expr.path)
    } else {
        None
    }
}

fn ident_from_path(mut path: Path) -> Option<Ident> {
    path.segments.pop().map(|seg| seg.into_value().ident)
}

fn sequence<'a>(
    deps: &BTreeMap<&'a Ident, BTreeSet<&Ident>>,
    mut precedance: BTreeSet<&'a Ident>,
) -> Option<SequenceDesc> {
    if precedance.len() < deps.len() {
        let mut ids = BTreeSet::new();
        let mut ids_str = BTreeSet::new();
        for (id, deps) in deps {
            if !precedance.contains(id) && precedance.is_superset(deps) {
                ids.insert(*id);
                ids_str.insert(id.to_string());
            }
        }
        precedance.append(&mut ids);
        Some(SequenceDesc {
            ids: ids_str,
            next: sequence(deps, precedance).map(Box::new),
        })
    } else {
        None
    }
}
