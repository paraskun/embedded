`timescale 1ns / 1ps

module tb_sum4();

  logic [3:0] a;
  logic [3:0] b;
  logic       cin;
  logic [3:0] s;
  logic       cout;
  
  sum4 DUT(.a(a), .b(b), .cin(cin), .s(s), .cout(cout), .p(), .g());
  
  initial begin
    // $dumpfile("out/sum4/sim.vcd");
    // $dumpvars(0, tb_sum4);

    {cin, a, b} = 9'b0;              #10;

    repeat (511) begin
      {cin, a, b} = {cin, a, b} + 1; #10;
    end
      
  $finish;
  end

endmodule
