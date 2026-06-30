use clap::Parser;
use md2pptx::cli::Args;

fn main() {
    let args = Args::parse();

    match md2pptx::convert(&args.input, &args.output, args.style.as_deref()) {
        Ok(warnings) => {
            md2pptx::diagnostics::print_warnings(&warnings);
        }
        Err(error) => {
            md2pptx::diagnostics::print_error(&error);
            std::process::exit(1);
        }
    }
}
