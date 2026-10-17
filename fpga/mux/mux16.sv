`timescale 1ns / 1ps

module mux16(
    input        [15:0] a,
    input         [3:0] s,
    output logic        f
    );

  logic [3:0] m;

  mux4 M0(.a(a[3:0]), .s(s[1:0]), .f(m[0]));
  mux4 M1(.a(a[7:4]), .s(s[1:0]), .f(m[1]));
  mux4 M2(.a(a[11:8]), .s(s[1:0]), .f(m[2]));
  mux4 M3(.a(a[15:12]), .s(s[1:0]), .f(m[3]));

  mux4 MR(.a(m), .s(s[3:2]), .f(f));
    
endmodule
