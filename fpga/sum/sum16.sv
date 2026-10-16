`timescale 1ns / 1ps

module sum16(
    input  [15:0] a,
    input  [15:0] b,
    input         cin,
    output [15:0] s,
    output        cout,
    output        p,
    output        g
    );

    logic [3:0] _cin;
    assign _cin[0] = cin;

    logic [3:0] _p;
    logic [3:0] _g;

    genvar i;
    generate
      for (i = 0; i < 4; i = i + 1) begin: blk
        sum4 S(.a(a[4*i+3:4*i]),
               .b(b[4*i+3:4*i]),
               .cin(_cin[i]),
               .s(s[4*i+3:4*i]),
               .cout(),
               .p(_p[i]),
               .g(_g[i]));
      end
    endgenerate

    cla4 C(.pin(_p), .gin(_g), .cin(cin), ._cin(_cin[3:1]), .cout(cout), .p(p), .g(g));

endmodule
