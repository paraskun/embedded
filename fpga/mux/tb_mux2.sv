`timescale 1ns / 1ps

module tb_mux2();

  logic [1:0] a;
  logic       s;
  logic       f;
  
  mux2 DUT(.a(a), .s(s), .f(f));
  
  initial begin
    $dumpfile("out/mux2/sim.vcd");
    $dumpvars(0, tb_mux2);

    s = 0; a = 2'b10; #10;
    s = 0; a = 2'b01; #10;
    s = 1; a = 2'b10; #10;
    s = 1; a = 2'b01; #10;
      
  $finish;
  end

endmodule
