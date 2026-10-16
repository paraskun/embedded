`timescale 1ns / 1ps

module tb_sum();

  logic a;
  logic b;
  logic cin;
  logic s;
  logic cout;
  
  sum DUT(.a(a), .b(b), .cin(cin), .s(s), .cout(cout), .p(), .g());
  
  initial begin
    // $dumpfile("out/sum/sim.vcd");
    // $dumpvars(0, tb_sum);

    {cin, a, b} = 3'b000;            #10;

    repeat (7) begin
      {cin, a, b} = {cin, a, b} + 1; #10;
    end
      
  $finish;
  end

endmodule
