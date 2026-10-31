`timescale 1ns / 1ps

module cnt(
    input              clk,
    input              res,
    output logic [3:0] q
    );

    always_ff @(posedge clk) begin
        if (res)
            q <= 4'b0;
        else
            q <= q + 1;
    end

endmodule