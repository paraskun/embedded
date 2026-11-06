`timescale 1ns / 1ps

module clocker #(
    parameter int unsigned DIV = 12_000_000
) (
    input  logic        i,
    input  logic        r,
    output logic [13:0] om,
    output logic [13:0] os
);

    logic [5:0] m;
    logic [5:0] s;

    clock #(
        .DIV(DIV)
    ) CLOCK (
        .i(i),
        .r(r),
        .m(m),
        .s(s)
    );

    decoder MINUTES (
        .i(m),
        .o(om)
    );

    decoder SECONDS (
        .i(s),
        .o(os)
    );

endmodule
