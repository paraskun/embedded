`timescale 1ns / 1ps

module sr(
    input              clk,
    input              sin,
    output logic [3:0] q
    );

    always @(posedge clk) begin
      q <= {q[2:0], sin};
    end
    
endmodule
