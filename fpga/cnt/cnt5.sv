`timescale 1ns / 1ps

module cnt5(
    input              clk,
    input              res,
    output logic [2:0] q
    );

    always_ff @(posedge clk) begin
        if (res)
            q <= 3'b0;
        else begin
            q[0] <= !q[2] & !q[1] & !q[0]
                  | !q[2] &  q[1] & !q[0];
            q[1] <= !q[2] & !q[1] &  q[0]
                  | !q[2] &  q[1] & !q[0];
            q[2] <= !q[2] &  q[1] &  q[0];
        end
    end

endmodule