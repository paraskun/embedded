`timescale 1ns / 1ps

module clock #(
    parameter int unsigned DIV = 12_000_000
) (
    input  logic       i,
    input  logic       r,
    output logic [5:0] m,
    output logic [5:0] s
);

    logic tick;

    div #(
        .DIV(DIV)
    ) DIVIDER (
        .i(i),
        .r(r),
        .o(tick)
    );

    always_ff @(posedge i or posedge r) begin
        if (r) begin
            m <= 6'b0;
            s <= 6'b0;
        end
        else if (tick) begin
            if (s == 6'd59 && m == 6'd59) begin
                m <= 6'b0;
                s <= 6'b0;
            end
            else if (s == 6'd59) begin
                m <= m + 6'b1;
                s <= 6'b0;
            end
            else begin
                s <= s + 6'b1;
            end
        end
    end

endmodule
