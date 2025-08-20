use spars::serve;
use spars::Settings;

fn main() {
    let settings = Settings::from_env().expect("Invalid Settings");

    serve(settings).expect("Failed to run server");
}
