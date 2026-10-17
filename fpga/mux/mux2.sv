`timescale 1ns / 1ps

module mux2(
    input        [1:0] a,
    input              s,
    output logic       f
    );

  // always_comb
  // if (s == 1'b0) f = a[0];
  // else           f = a[1];

  assign f = !s & a[0]
            | s & a[1];
    
endmodule
