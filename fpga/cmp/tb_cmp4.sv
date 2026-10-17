`timescale 1ns / 1ps

module tb_cmp4();

  logic [3:0] a;
  logic [3:0] b;
  logic       m;
  logic       l;
  
  cmp4 DUT(.a(a), .b(b), .m(m), .l(l));
  
  initial begin
    $dumpfile("out/cmp4/sim.vcd");
    $dumpvars(0, tb_cmp2);

    {a, b} = 8'b0;         #10;
    repeat (256) begin
      {a, b} = {a, b} + 1; #10;
    end
      
  $finish;
  end

endmodule
