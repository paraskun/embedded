`timescale 1ns / 1ps

module tb_cmp8();

  logic [7:0] a;
  logic [7:0] b;
  logic       m;
  logic       l;
  
  cmp8 DUT(.a(a), .b(b), .m(m), .l(l));
  
  initial begin
    $dumpfile("out/cmp8/sim.vcd");
    $dumpvars(0, tb_cmp8);

    {a, b} = 16'b0;        #10;
    repeat (65535) begin
      {a, b} = {a, b} + 1; #10;
    end
      
  $finish;
  end

endmodule
