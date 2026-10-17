`timescale 1ns / 1ps

module tb_sr();

  logic       clk;
  logic       sin;
  logic [3:0] q;
  
  sr DUT(.clk(clk), .sin(sin), .q(q));

  always begin
    clk = 0; #10;
    clk = 1; #10;
  end
  
  initial begin
    $dumpfile("out/sr/sim.vcd");
    $dumpvars(0, tb_sr);

    sin = 0; q = 4'b0101; #80;
    sin = 1; #20;
    sin = 0; #20;
    sin = 1; #20;
      
  $finish;
  end

endmodule
