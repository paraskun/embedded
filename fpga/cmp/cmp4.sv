`timescale 1ns / 1ps

module cmp4(
    input        [3:0] a,
    input        [3:0] b,
    output             m,
    output             l
    );

    logic [1:0] tm;
    logic [1:0] tl;

    // cmp2 C0(.a(a[1:0]), .b(b[1:0]), .m(tm[0]), .l(tl[0]);
    // cmp2 C1(.a(a[3:2]), .b(b[3:2]), .m(tm[1]), .l(tl[1]);
    // cmp2 CR(.a(tm), .b(tl), .m(m), .l(l));
    
    genvar i;
    generate
      for (i = 0; i < 2; i = i + 1)
      begin label
        cmp2 C0(
          .a(a[i * 2 + 1:i * 2]),
          .b(b[i * 2 + 1:i * 2]),
          .m(tm[i]),
          .l(tl[i]));
    endgenerate

    cmp2 CR(.a(tm), .b(tl), .m(m), .l(l));
    
endmodule
