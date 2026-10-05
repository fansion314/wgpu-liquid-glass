use std::{collections::HashSet, fs, path::Path};

fn collect_tokens(tokens: proc_macro2::TokenStream, chars: &mut HashSet<char>) {
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Group(group) => collect_tokens(group.stream(), chars),
            proc_macro2::TokenTree::Literal(literal) => {
                if let Ok(lit) = syn::parse_str::<syn::Lit>(&literal.to_string()) {
                    match lit {
                        syn::Lit::Str(s) => chars.extend(s.value().chars()),
                        syn::Lit::Char(c) => {
                            chars.insert(c.value());
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_sources(path: &Path, chars: &mut HashSet<char>) {
    println!("cargo:rerun-if-changed={}", path.display());
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_sources(&path, chars);
        } else if path.extension().is_some_and(|e| e == "rs") {
            println!("cargo:rerun-if-changed={}", path.display());
            collect_tokens(fs::read_to_string(path).unwrap().parse().unwrap(), chars);
        }
    }
}

fn main() {
    let mut chars: HashSet<char> = (' '..='~').collect();
    // Numeric controls and egui decorations can produce these at runtime.
    chars.extend("−×°…↔↕▶▼◀▲✓☑☐⏷⏵".chars());
    collect_sources(Path::new("src"), &mut chars);
    println!("cargo:rerun-if-changed=assets/font-extra.txt");
    chars.extend(fs::read_to_string("assets/font-extra.txt").unwrap().chars());
    println!("cargo:rerun-if-changed=assets/LXGWWenKai-Regular.ttf");
    let original = fs::read("assets/LXGWWenKai-Regular.ttf").unwrap();
    let subset =
        fontcull::subset_font_data(&original, &chars, &[]).expect("font subsetting failed");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("wenkai-ui.ttf"), &subset).unwrap();
    let mut chars: Vec<char> = chars.into_iter().collect();
    chars.sort_unstable();
    fs::write(
        out.join("font-characters.txt"),
        chars.into_iter().collect::<String>(),
    )
    .unwrap();
    println!(
        "cargo:warning=UI font subset: {} -> {} bytes",
        original.len(),
        subset.len()
    );
}
