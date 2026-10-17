`timescale 1ns / 1ps

module mux8(
    input        [7:0] a,
    input        [2:0] s,
    output logic       f
    );

  // always_comb
  // if      (s == 3'b000) f = a[0];
  // else if (s == 3'b001) f = a[1];
  // else if (s == 3'b010) f = a[2];
  // else if (s == 3'b011) f = a[3];
  // else if (s == 3'b100) f = a[4];
  // else if (s == 3'b101) f = a[5];
  // else if (s == 3'b110) f = a[6];
  // else                  f = a[7];
  
  // always_comb
  // case (s)
  // 3'b000: f = a[0];
  // 3'b001: f = a[1];
  // 3'b010: f = a[2];
  // 3'b011: f = a[3];
  // 3'b100: f = a[4];
  // 3'b101: f = a[5];
  // 3'b110: f = a[6];
  // 3'b111: f = a[7];
  // endcase

  assign f = !s[2] & !s[1] & !s[0] & a[0]
           | !s[2] & !s[1] &  s[0] & a[1]
           | !s[2] &  s[1] & !s[0] & a[2]
           | !s[2] &  s[1] &  s[0] & a[3]
           |  s[2] & !s[1] & !s[0] & a[4]
           |  s[2] & !s[1] &  s[0] & a[5]
           |  s[2] &  s[1] & !s[0] & a[6]
           |  s[2] &  s[1] &  s[0] & a[7];
    
endmodule
