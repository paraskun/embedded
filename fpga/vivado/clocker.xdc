# Digilent Cmod A7-35T constraints for the clocker design.
#
# The RTL segment order is abcdefg. For each 14-bit output:
#   [13:7] = tens digit a,b,c,d,e,f,g
#   [ 6:0] = ones digit a,b,c,d,e,f,g
#
# The decoder outputs are active-high. Use a current-limiting resistor in
# series with every segment. If the board is different, change only the
# PACKAGE_PIN values below.

# 12 MHz oscillator (Cmod A7 pin GCLK).
set_property -dict {PACKAGE_PIN L17 IOSTANDARD LVCMOS33} [get_ports i]
create_clock -name sys_clk -period 83.333 [get_ports i]

# Button BTN0, used as the active-high asynchronous reset.
set_property -dict {PACKAGE_PIN A18 IOSTANDARD LVCMOS33} [get_ports r]
set_false_path -from [get_ports r]

# Configuration bank voltage for Cmod A7.
set_property CFGBVS VCCO [current_design]
set_property CONFIG_VOLTAGE 3.3 [current_design]

# Minutes tens digit, segments a through g.
set_property -dict {PACKAGE_PIN M3  IOSTANDARD LVCMOS33} [get_ports {om[13]}]; # PIO1  - tens a
set_property -dict {PACKAGE_PIN L3  IOSTANDARD LVCMOS33} [get_ports {om[12]}]; # PIO2  - tens b
set_property -dict {PACKAGE_PIN A16 IOSTANDARD LVCMOS33} [get_ports {om[11]}]; # PIO3  - tens c
set_property -dict {PACKAGE_PIN K3  IOSTANDARD LVCMOS33} [get_ports {om[10]}]; # PIO4  - tens d
set_property -dict {PACKAGE_PIN C15 IOSTANDARD LVCMOS33} [get_ports {om[9]}];  # PIO5  - tens e
set_property -dict {PACKAGE_PIN H1  IOSTANDARD LVCMOS33} [get_ports {om[8]}];  # PIO6  - tens f
set_property -dict {PACKAGE_PIN A15 IOSTANDARD LVCMOS33} [get_ports {om[7]}];  # PIO7  - tens g

# Minutes ones digit, segments a through g.
set_property -dict {PACKAGE_PIN B15 IOSTANDARD LVCMOS33} [get_ports {om[6]}];  # PIO8  - ones a
set_property -dict {PACKAGE_PIN A14 IOSTANDARD LVCMOS33} [get_ports {om[5]}];  # PIO9  - ones b
set_property -dict {PACKAGE_PIN J3  IOSTANDARD LVCMOS33} [get_ports {om[4]}];  # PIO10 - ones c
set_property -dict {PACKAGE_PIN J1  IOSTANDARD LVCMOS33} [get_ports {om[3]}];  # PIO11 - ones d
set_property -dict {PACKAGE_PIN K2  IOSTANDARD LVCMOS33} [get_ports {om[2]}];  # PIO12 - ones e
set_property -dict {PACKAGE_PIN L1  IOSTANDARD LVCMOS33} [get_ports {om[1]}];  # PIO13 - ones f
set_property -dict {PACKAGE_PIN L2  IOSTANDARD LVCMOS33} [get_ports {om[0]}];  # PIO14 - ones g

# Seconds tens digit, segments a through g.
set_property -dict {PACKAGE_PIN M1 IOSTANDARD LVCMOS33} [get_ports {os[13]}];  # PIO17 - tens a
set_property -dict {PACKAGE_PIN N3 IOSTANDARD LVCMOS33} [get_ports {os[12]}];  # PIO18 - tens b
set_property -dict {PACKAGE_PIN P3 IOSTANDARD LVCMOS33} [get_ports {os[11]}];  # PIO19 - tens c
set_property -dict {PACKAGE_PIN M2 IOSTANDARD LVCMOS33} [get_ports {os[10]}];  # PIO20 - tens d
set_property -dict {PACKAGE_PIN N1 IOSTANDARD LVCMOS33} [get_ports {os[9]}];   # PIO21 - tens e
set_property -dict {PACKAGE_PIN N2 IOSTANDARD LVCMOS33} [get_ports {os[8]}];   # PIO22 - tens f
set_property -dict {PACKAGE_PIN P1 IOSTANDARD LVCMOS33} [get_ports {os[7]}];   # PIO23 - tens g

# Seconds ones digit, segments a through g.
set_property -dict {PACKAGE_PIN R3 IOSTANDARD LVCMOS33} [get_ports {os[6]}];   # PIO26 - ones a
set_property -dict {PACKAGE_PIN T3 IOSTANDARD LVCMOS33} [get_ports {os[5]}];   # PIO27 - ones b
set_property -dict {PACKAGE_PIN R2 IOSTANDARD LVCMOS33} [get_ports {os[4]}];   # PIO28 - ones c
set_property -dict {PACKAGE_PIN T1 IOSTANDARD LVCMOS33} [get_ports {os[3]}];   # PIO29 - ones d
set_property -dict {PACKAGE_PIN T2 IOSTANDARD LVCMOS33} [get_ports {os[2]}];   # PIO30 - ones e
set_property -dict {PACKAGE_PIN U1 IOSTANDARD LVCMOS33} [get_ports {os[1]}];   # PIO31 - ones f
set_property -dict {PACKAGE_PIN W2 IOSTANDARD LVCMOS33} [get_ports {os[0]}];   # PIO32 - ones g
