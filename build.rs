use quote::quote;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs, path::Path};

#[path = "src/figure.rs"]
mod figure;

use figure::Figure;

const DIGITS_TXT: &str = r"
###| #|###|###|# #|###|###|###|###|###
# #|##|  #|  #|# #|#  |#  |  #|# #|# #
# #| #|###|###|###|###|###|  #|###|###
# #| #|#  |  #|  #|  #|# #|  #|# #|  #
###| #|###|###|  #|###|###|  #|###|###
";

const TETRAMION_TXT: &str = r"
##| # |  #|#  | ##|## |
##|###|###|###|## | ##|####
";

struct FiguresExt {
    figures: Vec<Figure>,
}

impl quote::ToTokens for FiguresExt {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let figures: Vec<_> = self
            .figures
            .iter()
            .map(|f| {
                let data = f.data;
                let wh = f.wh;
                quote! {
                    Figure{data: #data, wh: #wh}
                }
            })
            .collect();

        let figures = quote! {
            #(#figures),*
        };
        tokens.extend(figures);
    }
}

fn split_figures_txt(text: &str) -> Vec<String> {
    let mut figures_txt: Vec<String> = Vec::new();
    for (idx, line) in text.trim().lines().enumerate() {
        if idx == 0 {
            figures_txt.extend(line.split("|").map(|s| s.to_string()));
        } else {
            line.split("|").enumerate().for_each(|(idx, part)| {
                figures_txt[idx].push('\n');
                figures_txt[idx].push_str(part);
            });
        }
    }
    figures_txt
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("figures.rs");
    let digits = FiguresExt {
        figures: Vec::from_iter(
            split_figures_txt(DIGITS_TXT)
                .iter()
                .map(|s| Figure::from_str(s.as_str())),
        ),
    };

    let tetramino = FiguresExt {
        figures: Vec::from_iter(
            split_figures_txt(TETRAMION_TXT)
                .iter()
                .map(|s| Figure::from_str(s.as_str())),
        ),
    };

    let mut code = quote! { const DIGITS: Digits = Digits::new([ #digits ]); }.to_string();

    code.push_str(
        quote! { const TETRAMINO: Tetramino = Tetramino::new([ #tetramino ]); }
            .to_string()
            .as_str(),
    );
    code.push_str(
        quote! {
            const HLINE: Figure = Figure {
                data: 0xff,
                wh: 8 << 4 | 1,
            };
        }
        .to_string()
        .as_str(),
    );

    println!("cargo:rerun-if-changed=build.rs");

    let file = syn::parse_file(&code).unwrap();
    fs::write(&dest_path, prettyplease::unparse(&file)).unwrap();

    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=memory.x");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
