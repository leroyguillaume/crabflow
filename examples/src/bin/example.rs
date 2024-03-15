extern crate crabflow;
extern crate crabflow_examples;

use crabflow::workflow;

#[workflow]
fn example() {
    #[task]
    fn a() {
        println!("a");
    }

    #[task]
    fn b() {
        println!("b");
    }

    #[task]
    fn c() {
        println!("c");
    }
}
