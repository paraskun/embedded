`timescale 1ns / 1ps

module sr8(
  input            clk,
  input            load,
  input            sin,
  input      [7:0] d,
  output reg [7:0] q
  );

  always @(posedge clk) begin
    if (load)
      q <= d;
    else
      q <= {q[6:0], sin};
  end

endmodule
