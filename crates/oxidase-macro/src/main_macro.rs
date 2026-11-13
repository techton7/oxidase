//! Procedural macro attribute for transparent cross-platform app launching with zero-wiring frame support.

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;

fn is_launch_call(func: &syn::Expr) -> bool {
    if let syn::Expr::Path(expr_path) = func {
        let idents: Vec<String> = expr_path
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();

        if idents == ["dioxus", "launch"] || idents == ["launch"] || idents == ["oxidase", "launch"] {
            return true;
        }
        if let Some(last) = expr_path.path.segments.last()
            && last.ident == "launch" {
                return true;
            }
    }
    false
}

fn rewrite_launch_expr(expr: &mut syn::Expr, is_headless: bool) -> bool {
    match expr {
        syn::Expr::Call(call) => {
            if is_launch_call(&call.func) {
                let app_expr = call.args.first().cloned();
                if let Some(app) = app_expr {
                    if is_headless {
                        *expr = syn::parse_quote!(oxidase::launch(#app));
                    } else {
                        *expr = syn::parse_quote!(::oxidase::launch::launch_hosted(#app));
                    }
                    return true;
                }
            }
        }
        syn::Expr::Block(block) => {
            return rewrite_stmts(&mut block.block.stmts, is_headless);
        }
        _ => {}
    }
    false
}

fn rewrite_stmts(stmts: &mut [syn::Stmt], is_headless: bool) -> bool {
    let mut rewritten = false;
    for stmt in stmts.iter_mut() {
        if let syn::Stmt::Expr(expr, _semi) = stmt
            && rewrite_launch_expr(expr, is_headless) {
                rewritten = true;
            }
    }
    rewritten
}

pub fn expand_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_str = attr.to_string();
    let is_headless = attr_str.contains("headless");

    let mut item_fn = parse_macro_input!(item as syn::ItemFn);
    rewrite_stmts(&mut item_fn.block.stmts, is_headless);
    item_fn.to_token_stream().into()
}
