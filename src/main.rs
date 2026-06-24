use clap::Parser;
use md2pptx::cli::Args;

fn main() {
    let args = Args::parse();
    match md2pptx::convert(&args.input, &args.output, args.style.as_deref()) {
        Ok(warnings) => {
            for warning in warnings {
                eprintln!("warning: {warning}");
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
