`timescale 1ns / 1ps

// Generates a one-input-clock pulse every DIV input clocks.
// With a 12 MHz input and DIV = 12,000,000, o is a 1 Hz tick.
module div #(
    parameter int unsigned DIV = 12_000_000
) (
    input  logic i,
    input  logic r,
    output logic o
);

    localparam int WIDTH = (DIV > 1) ? $clog2(DIV) : 1;
    localparam logic [WIDTH-1:0] LAST = DIV - 1;

    logic [WIDTH-1:0] c;

    always_ff @(posedge i or posedge r) begin
        if (r) begin
            c <= '0;
            o <= 1'b0;
        end
        else if (c == LAST) begin
            c <= '0;
            o <= 1'b1;
        end
        else begin
            c <= c + 1'b1;
            o <= 1'b0;
        end
    end

endmodule
