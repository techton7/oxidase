//! Procedural macro attribute for rewriting `dioxus::launch(...)` into `::oxidase::launch(...)`.

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;

fn is_dioxus_launch(func: &syn::Expr) -> bool {
    if let syn::Expr::Path(expr_path) = func {
        let idents: Vec<String> = expr_path
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();

        if idents == ["dioxus", "launch"] || idents == ["launch"] {
            return true;
        }
        if let Some(last) = expr_path.path.segments.last() {
            if last.ident == "launch" {
                return true;
            }
        }
    }
    false
}

fn rewrite_launch_expr(expr: &mut syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => {
            if is_dioxus_launch(&call.func) {
                *call.func = syn::parse_quote!(oxidase::launch);
                return true;
            }
        }
        syn::Expr::Block(block) => {
            return rewrite_stmts(&mut block.block.stmts);
        }
        _ => {}
    }
    false
}

fn rewrite_stmts(stmts: &mut [syn::Stmt]) -> bool {
    let mut rewritten = false;
    for stmt in stmts.iter_mut() {
        if let syn::Stmt::Expr(expr, _semi) = stmt {
            if rewrite_launch_expr(expr) {
                rewritten = true;
            }
        }
    }
    rewritten
}

pub fn expand_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_fn = parse_macro_input!(item as syn::ItemFn);
    rewrite_stmts(&mut item_fn.block.stmts);
    item_fn.to_token_stream().into()
}
