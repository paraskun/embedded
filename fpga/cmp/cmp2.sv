`timescale 1ns / 1ps

module cmp2(
    input        [1:0] a,
    input        [1:0] b,
    output             m,
    output             l
    );

    assign m =  a[1] & !b[1]
             | !a[1] & !b[1] & a[0] & !b[0]
             |  a[1] &  b[1] & a[0] & !b[0];
    assign l =  b[1] & !a[1]
             | !b[1] & !a[1] & b[0] & !a[0]
             |  b[1] &  a[1] & b[0] & !a[0];
    
endmodule
