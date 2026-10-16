`timescale 1ns / 1ps

module sum64(
    input  [63:0] a,
    input  [63:0] b,
    input         cin,
    output [63:0] s,
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
        sum16 S(.a(a[16*i+15:16*i]),
               .b(b[16*i+15:16*i]),
               .cin(_cin[i]),
               .s(s[16*i+15:16*i]),
               .cout(),
               .p(_p[i]),
               .g(_g[i]));
      end
    endgenerate

    cla4 C(.pin(_p), .gin(_g), .cin(cin), ._cin(_cin[3:1]), .cout(cout), .p(p), .g(g));

endmodule
