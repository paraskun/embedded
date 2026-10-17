`timescale 1ns / 1ps

module tb_cmp2();

  logic [1:0] a;
  logic [1:0] b;
  logic       m;
  logic       l;
  
  cmp2 DUT(.a(a), .b(b), .m(m), .l(l));
  
  initial begin
    $dumpfile("out/cmp2/sim.vcd");
    $dumpvars(0, tb_cmp2);

    {a, b} = 4'b0;         #10;
    repeat (15) begin
      {a, b} = {a, b} + 1; #10;
    end
      
  $finish;
  end

endmodule
