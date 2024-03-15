use std::collections::{BTreeMap, BTreeSet};

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
    let funs = workflow.tasks.values().map(|task| &task.fun);
    let enum_values = workflow.tasks.values().map(|task| {
        let id = &task.id;
        let about = format!("Run task `{id}`");
        quote! {
            #[command(about = #about)]
            #id
        }
    });
    let run_cases = workflow.tasks.values().map(|task| {
        let id = &task.id;
        let fun_name = &task.fun.sig.ident;
        quote! {
            Self::#id => #fun_name()
        }
    });
    quote! {
        fn main() {
            use crabflow::{clap::Parser, core::*};
            use crabflow_internal::*;

            let args = Args::parse();
            args.task.run();
        }

        #(#funs)*

        mod crabflow_internal {
            use crabflow::clap::{Parser, Subcommand, self};

            use super::*;

            #[derive(Clone, Debug, Eq, Parser, PartialEq)]
            #[command(author)]
            pub struct Args {
                #[command(subcommand)]
                pub task: Task,
            }

            #[derive(Clone, Debug, Eq, PartialEq, Subcommand)]
            #[allow(non_camel_case_types)]
            pub enum Task {
                #(#enum_values),*
            }

            impl Task {
                pub fn run(self) {
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
    deps: BTreeSet<Ident>,
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
    deps: Option<BTreeSet<Ident>>,
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
                .ok_or_else(|| Error::new(span, "parameter should be ident"))?;
            if key == "id" {
                let value = ident_from_expr(*param.right)
                    .ok_or_else(|| Error::new(span, "id should be ident"))?;
                id = Some(value);
            } else if key == "depends_on" {
                let tasks = match *param.right {
                    Expr::Array(expr) => expr
                        .elems
                        .into_iter()
                        .map(|expr| {
                            ident_from_expr(expr).ok_or_else(|| {
                                Error::new(span, "depends_on should be ident or array of idents")
                            })
                        })
                        .collect::<Result<BTreeSet<Ident>>>(),
                    Expr::Path(expr) => {
                        let task = ident_from_path(expr.path).ok_or_else(|| {
                            Error::new(span, "depends_on should be ident or array of idents")
                        })?;
                        Ok(BTreeSet::from_iter([task]))
                    }
                    _ => Err(Error::new(
                        span,
                        "depends_on should be ident or array of idents",
                    )),
                }?;
                deps = Some(tasks);
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
    tasks: BTreeMap<Ident, Task>,
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
        let mut tasks = BTreeMap::new();
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
                    let task = task.apply(params);
                    tasks.insert(task.id.clone(), task);
                }
            }
        }
        for task in tasks.values() {
            for dep in &task.deps {
                if !tasks.contains_key(dep) {
                    let msg = format!("task `{dep}` doesn't exist");
                    return Err(Error::new(task.span, msg));
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

fn get_attr(key: &str, fun: &mut ItemFn) -> Option<Attribute> {
    fun.attrs
        .iter()
        .position(|attr| attr.path().is_ident(key))
        .map(|idx| fun.attrs.remove(idx))
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
