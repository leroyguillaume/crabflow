use std::{
    fs::{create_dir_all, File},
    path::Path,
};

use crabflow_core::{TasksChain, Workflow};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse_macro_input, punctuated::Punctuated, Attribute, Expr, ExprAssign, Ident, Item, ItemFn,
    Meta, Stmt, Token,
};

struct Task {
    depends_on: Vec<Ident>,
    enum_value: TokenStream2,
    function: ItemFn,
    id: String,
    run_case: TokenStream2,
}

impl Task {
    fn parse(function: ItemFn, attr: Attribute) -> Self {
        let fn_name = &function.sig.ident;
        let mut task_id = fn_name.clone();
        let mut depends_on = vec![];
        if let Meta::List(args) = attr.meta {
            let tokens = TokenStream::from(args.tokens);
            let args = parse_macro_input!(tokens with Punctuated::<ExprAssign, Token![,]>::parse_terminated);
            for arg in args {
                if let Expr::Path(key) = *arg.left {
                    if key.path.is_ident("id") {
                        if let Expr::Path(id) = *arg.right {
                            if let Some(id) = id.path.get_ident() {
                                task_id = id.clone();
                            } else {
                                panic!("task `{task_id}`: id should be ident");
                            }
                        }
                    } else if key.path.is_ident("depends_on") {
                        match *arg.right {
                            Expr::Array(tasks) => {
                                depends_on = vec![];
                                for task in tasks.elems {
                                    if let Expr::Path(task) = task {
                                        if let Some(task) = task.path.get_ident() {
                                            depends_on.push(task.clone());
                                        } else {
                                            panic!("task `{task_id}`: depends_on should be ident or array of idents");
                                        }
                                    } else {
                                        panic!("task `{task_id}`: depends_on should be ident or array of idents");
                                    }
                                }
                            }
                            Expr::Path(task) => {
                                if let Some(task) = task.path.get_ident() {
                                    depends_on = vec![task.clone()];
                                } else {
                                    panic!("task `{task_id}`: depends_on should be ident or array of idents");
                                }
                            }
                            _ => {
                                panic!("task `{task_id}`: depends_on should be ident or array of idents");
                            }
                        }
                    } else if let Some(key) = key.path.get_ident() {
                        panic!("task `{task_id}`: unknwon argument `{key}`");
                    } else {
                        panic!("task `{task_id}`: unknwon argument");
                    }
                } else {
                    panic!("task `{task_id}`: invalid arguments, should be key = value");
                }
            }
        }
        let about = format!("Run task `{task_id}`");
        Task {
            depends_on,
            enum_value: quote! {
                #[command(about = #about)]
                #task_id
            },
            function,
            id: task_id.to_string(),
            run_case: quote! {
                Task::#task_id => #fn_name()
            },
        }
    }
}

#[proc_macro_attribute]
pub fn workflow(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let workflow_fn = parse_macro_input!(input as ItemFn);
    let mut tasks = vec![];
    for statement in workflow_fn.block.stmts {
        if let Stmt::Item(Item::Fn(mut function)) = statement {
            let attr = function
                .attrs
                .iter()
                .enumerate()
                .find(|(_, attr)| attr.path().is_ident("task"))
                .map(|(idx, attr)| (idx, attr.clone()));
            if let Some((idx, attr)) = attr {
                function.attrs.remove(idx);
                tasks.push(Task::parse(function, attr));
            }
        }
    }
    let mut workflow = Workflow {
        chain: TasksChain::default(),
        id: format!("{}", workflow_fn.sig.ident),
    };
    let dir = Path::new("target/workflows");
    let path = dir.join(format!("{}.json", workflow.id));
    create_dir_all(dir).expect("failed to create workflows directory");
    let mut file = File::create(path).expect("failed to create workflow file");
    serde_json::to_writer(&mut file, &workflow).expect("failed to write workflow file");
    let enum_values = tasks.iter().map(|task| task.enum_value);
    let functions = tasks.iter().map(|task| task.function);
    let run_cases = tasks.iter().map(|task| task.run_case);
    quote! {
        fn main() {
            use crabflow::{clap::Parser, core::*};
            use crabflow_internal::*;

            let args = Args::parse();
            args.task.run();
        }

        #(#functions)*

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
