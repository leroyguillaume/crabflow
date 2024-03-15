extern crate crabflow;
extern crate crabflow_examples;

use crabflow::{workflow, Result};

#[workflow]
fn example() {
    #[task]
    async fn a() -> Result {
        println!("a");
        Ok(())
    }

    #[task(depends_on = a)]
    async fn b() -> Result {
        println!("b");
        Ok(())
    }

    #[task(depends_on = b)]
    async fn c() -> Result {
        println!("c");
        Ok(())
    }
}
