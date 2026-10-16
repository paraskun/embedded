`timescale 1ns / 1ps

module cla4(
    input  [3:0] pin,
    input  [3:0] gin,
    input        cin,
    output [3:1] _cin,
    output       cout,
    output       p,
    output       g
    );

    assign _cin[1] = gin[0]
                   | pin[0] & cin;

    assign _cin[2] = gin[1]
                   | pin[1] & gin[0]
                   | pin[1] & pin[0] & cin;

    assign _cin[3] = gin[2]
                   | pin[2] & gin[1]
                   | pin[2] & pin[1] & gin[0]
                   | pin[2] & pin[1] & pin[0] & cin;

    assign p = pin[3] & pin[2] & pin[1] & pin[0];

    assign g = gin[3]
             | pin[3] & gin[2]
             | pin[3] & pin[2] & gin[1]
             | pin[3] & pin[2] & pin[1] & gin[0];

    assign cout = g | (p & cin);
    
endmodule
