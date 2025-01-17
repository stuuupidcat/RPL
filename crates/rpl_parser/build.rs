use quote::quote;
use std::fs::File;
use std::io::Write as _;
use std::process::{Command, Stdio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=src/grammar/RPL.pest");
    let parser = {
        use pest_typed_generator::derive_typed_parser;
        let input = quote! {
            /// Underlying definition of the parser written with Pest.
            #[derive(TypedParser)]
            #[grammar = "grammar/RPL.pest"]
            #[emit_rule_reference]
            pub struct Grammar;
        };
        derive_typed_parser(input.clone(), false, true)
    };

    let child = Command::new("rustfmt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.as_ref().unwrap();
    stdin.write_all(
        b"#![allow(warnings)]\n/// Underlying definition of the RPL parser written with Pest.\npub struct Grammar;",
    )?;
    write!(stdin, "{}", parser)?;
    let output = child.wait_with_output()?;
    File::create("src/parser.rs")?.write_all(&output.stdout)?;

    Ok(())
}
