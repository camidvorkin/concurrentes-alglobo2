use serde_yaml::{self, Sequence};

fn main() -> Result<(), Box<std::error::Error>> {
    let f = std::fs::File::open("src/agents.yaml")?;
    let d: Sequence = serde_yaml::from_reader(f)?;
    for agent in d {
        println!("{}", agent.get("name").unwrap().as_str().unwrap());
        println!("{}", agent.get("successrate").unwrap().as_f64().unwrap());
        println!("{}", agent.get("port").unwrap().as_i64().unwrap());
    }
    Ok(())
}
