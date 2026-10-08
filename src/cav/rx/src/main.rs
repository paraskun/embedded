#![no_main]
#![no_std]

mod dpp;

use embassy_executor::Spawner;
use panic_halt as _;

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    // Your code here
}