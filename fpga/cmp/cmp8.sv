`timescale 1ns / 1ps

module cmp8(
    input        [7:0] a,
    input        [7:0] b,
    output             m,
    output             l
    );

    // -- SEQUENTIAL
    // 
    // logic [5:0] a0;
    // logic [5:0] b0;

    // cmp2 C0(.a(a[1:0]),        .b(b[1:0]),        .m(a0[0]), .l(b0[0]));
    // cmp2 C1(.a({a[2], a0[0]}), .b({b[2], b0[0]}), .m(a0[1]), .l(b0[1]));
    // cmp2 C2(.a({a[3], a0[1]}), .b({b[3], b0[1]}), .m(a0[2]), .l(b0[2]));
    // cmp2 C3(.a({a[4], a0[2]}), .b({b[4], b0[2]}), .m(a0[3]), .l(b0[3]));
    // cmp2 C4(.a({a[5], a0[3]}), .b({b[5], b0[3]}), .m(a0[4]), .l(b0[4]));
    // cmp2 C5(.a({a[6], a0[4]}), .b({b[6], b0[4]}), .m(a0[5]), .l(b0[5]));
    // cmp2 C6(.a({a[7], a0[5]}), .b({b[7], b0[5]}), .m(m), .l(l));

    // -- PARALLEL --
    
    logic [3:0] a1;
    logic [3:0] b1;
    logic [1:0] a2;
    logic [1:0] b2;
    
    cmp2 C0(.a(a[1:0]), .b(b[1:0]), .m(a1[0]), .l(b1[0]));
    cmp2 C1(.a(a[3:2]), .b(b[3:2]), .m(a1[1]), .l(b1[1]));
    cmp2 C2(.a(a[5:4]), .b(b[5:4]), .m(a1[2]), .l(b1[2]));
    cmp2 C3(.a(a[7:6]), .b(b[7:6]), .m(a1[3]), .l(b1[3]));
    
    cmp2 C4(.a(a1[1:0]), .b(b1[1:0]), .m(a2[0]), .l(b2[0]));
    cmp2 C5(.a(a1[3:2]), .b(b1[3:2]), .m(a2[1]), .l(b2[1]));
    
    cmp2 CR(.a(a2), .b(b2), .m(m), .l(l));

    // -- GENERATE --
    //
    // genvar j, i;
    // generate
    //   for (j = 0; j < 2; j = j + 1) begin: g2
    //     for (i = 0; i < 2; i = i + 1) begin: g1
    //       cmp2 c0(.a(a[j*4+i*2+1 : j*4+i*2]), .b(b[j*4+i*2+1 : j*4+i*2]), .m(a1[j*2+i]), .l(b1[j*2+i]));
    //     end
    //
    //     cmp2 c1(.a(a1[j*2+1 : j*2]), .b(b1[j*2+1 : j*2]), .m(a2[j]), .l(b2[j]));
    //   end
    // endgenerate
    //
    // cmp2 CR(.a(a2), .b(b2), .m(m), .l(l));
    
endmodule
