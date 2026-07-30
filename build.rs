use vergen_gitcl::{Emitter, Gitcl};

fn main() {
    let gitcl = Gitcl::builder().sha(true).commit_date(true).build();
    let _ = Emitter::default()
        .add_instructions(&gitcl)
        .and_then(|e| e.emit());
}
