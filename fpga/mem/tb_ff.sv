`timescale 1ns / 1ps

module tb_ff();

  logic clk;
  logic d;
  logic q;
  
  ff DUT(.clk(clk), .d(d), .q(q));

  always begin
    clk = 0; #10;
    clk = 1; #10;
  end
  
  initial begin
    $dumpfile("out/ff/sim.vcd");
    $dumpvars(0, tb_ff);

    d = 0; #20;
    d = 1; #20;
    d = 0; #20;
    d = 1; #20;
      
  $finish;
  end

endmodule
