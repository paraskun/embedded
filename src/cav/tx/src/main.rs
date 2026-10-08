#![no_std]
#![no_main]

mod led;

use embassy_executor::Spawner;
use panic_halt as _;

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry",
)]
async fn main(_spawner: Spawner) {
    // Your code here
}