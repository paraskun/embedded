`timescale 1ns / 1ps

module sum64x3(
    input  [63:0] a,
    input  [63:0] b,
    input  [63:0] c,
    input         cin,
    output [63:0] s,
    output  [1:0] cout
    );

    logic [63:0] _t;
    logic        _c1;
    logic        _c2;

    sum64 S0(.a(a), .b(b), .cin(cin), .s(_t), .cout(_c1), .p(), .g());
    sum64 S1(.a(_t), .b(c), .cin(1'b0), .s(s), .cout(_c2), .p(), .g());

    sum S(.a(_c1), .b(_c2), .cin(1'b0), .s(cout[0]), .cout(cout[1]), .p(), .g());

endmodule
