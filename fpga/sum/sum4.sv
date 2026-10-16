`timescale 1ns / 1ps

module sum4(
    input  [3:0] a,
    input  [3:0] b,
    input        cin,
    output [3:0] s,
    output       cout,
    output       p,
    output       g
    );

    // carry-to-ripple

    // logic [3:0] _cin;
    // assign _cin[0] = cin;

    // sum S0(.a(a[0]), .b(b[0]), .cin(_cin[0]), .s(s[0]), .cout(_cin[1]), .p(), .g());
    // sum S1(.a(a[1]), .b(b[1]), .cin(_cin[1]), .s(s[1]), .cout(_cin[2]), .p(), .g());
    // sum S2(.a(a[2]), .b(b[2]), .cin(_cin[2]), .s(s[2]), .cout(_cin[3]), .p(), .g());
    // sum S3(.a(a[3]), .b(b[3]), .cin(_cin[3]), .s(s[3]), .cout(cout), .p(), .g());

    // carry-lookahead

    logic [3:0] _cin;
    assign _cin[0] = cin;

    logic [3:0] _p;
    logic [3:0] _g;

    sum S0(.a(a[0]), .b(b[0]), .cin(_cin[0]), .s(s[0]), .cout(), .p(_p[0]), .g(_g[0]));
    sum S1(.a(a[1]), .b(b[1]), .cin(_cin[1]), .s(s[1]), .cout(), .p(_p[1]), .g(_g[1]));
    sum S2(.a(a[2]), .b(b[2]), .cin(_cin[2]), .s(s[2]), .cout(), .p(_p[2]), .g(_g[2]));
    sum S3(.a(a[3]), .b(b[3]), .cin(_cin[3]), .s(s[3]), .cout(), .p(_p[3]), .g(_g[3]));

    cla4 C(.pin(_p), .gin(_g), .cin(cin), ._cin(_cin[3:1]), .cout(cout), .p(p), .g(g));
    
endmodule
