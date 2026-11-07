# Headless Vivado build for the clocker design.
# Environment variables:
#   FPGA_PART - Xilinx FPGA part, for example xc7a35tcpg236-1
#   CLOCK_DIV - input clocks per 1 Hz tick
#   BUILD_DIR - output directory

proc env_or {name default_value} {
    if {[info exists ::env($name)] && $::env($name) ne ""} {
        return $::env($name)
    }
    return $default_value
}

set script_dir [file dirname [file normalize [info script]]]
set root_dir   [file normalize [file join $script_dir ..]]
set build_dir  [env_or BUILD_DIR [file join $root_dir out vivado clocker]]
set fpga_part  [env_or FPGA_PART xc7a35tcpg236-1]
set clock_div  [env_or CLOCK_DIV 12000000]
set top        clocker

file mkdir $build_dir

set sources [list \
    [file join $root_dir clk div.sv] \
    [file join $root_dir clk decoder.sv] \
    [file join $root_dir clk clock.sv] \
    [file join $root_dir clk clocker.sv] \
]

read_verilog -sv $sources
read_xdc [file join $script_dir clocker.xdc]

synth_design -top $top -part $fpga_part -generic "DIV=$clock_div"
opt_design
place_design
route_design

report_utilization      -file [file join $build_dir utilization.rpt]
report_timing_summary   -file [file join $build_dir timing_summary.rpt]
report_drc              -file [file join $build_dir drc.rpt]
write_checkpoint -force      [file join $build_dir $top.dcp]
write_bitstream  -force      [file join $build_dir $top.bit]

puts "Bitstream written to [file join $build_dir $top.bit]"
