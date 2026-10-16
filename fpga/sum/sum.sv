`timescale 1ns / 1ps

module sum(
    input  a,
    input  b,
    input  cin,
    output s,
    output cout,
    output p,
    output g
    );

    assign p = a ^ b;
    assign g = a & b;
    assign s = p ^ cin;
    assign cout = g | (p & cin);
    
endmodule
