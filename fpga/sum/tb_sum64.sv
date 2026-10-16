`timescale 1ns / 1ps
 
module tb_sum64();
 
  logic [63:0] a;
  logic [63:0] b;
  logic        cin;
  logic [63:0] s;
  logic        cout;
 
  sum64 DUT(.a(a), .b(b), .cin(cin), .s(s), .cout(cout), .p(), .g());
 
  integer      errors = 0;
  integer      tests  = 0;
  logic [64:0] expected;
 
  task automatic check(input logic [63:0] ta, input logic [63:0] tb, input logic tc);
    begin
      a   = ta;
      b   = tb;
      cin = tc;

      #1;

      expected = {1'b0, ta} + {1'b0, tb} + tc;
      tests = tests + 1;

      if ({cout, s} !== expected) begin
        errors = errors + 1;

        $display("FAIL: a=%h b=%h cin=%b -> got %b_%h, expected %b_%h",
                 ta, tb, tc, cout, s, expected[64], expected[63:0]);
      end
    end
  endtask
 
  initial begin
    // $dumpfile("out/sum64/sim.vcd");
    // $dumpvars(0, tb_sum64);
 
    check(64'h0,                   64'h0,                   1'b0);
    check(64'h0,                   64'h0,                   1'b1);
    check(64'hFFFF_FFFF_FFFF_FFFF, 64'h1,                   1'b0);
    check(64'hFFFF_FFFF_FFFF_FFFF, 64'h0,                   1'b1);
    check(64'hFFFF_FFFF_FFFF_FFFF, 64'hFFFF_FFFF_FFFF_FFFF, 1'b0);
    check(64'hFFFF_FFFF_FFFF_FFFF, 64'hFFFF_FFFF_FFFF_FFFF, 1'b1);
    check(64'h5555_5555_5555_5555, 64'hAAAA_AAAA_AAAA_AAAA, 1'b0);
    check(64'h5555_5555_5555_5555, 64'hAAAA_AAAA_AAAA_AAAA, 1'b1);
    check(64'h8000_0000_0000_0000, 64'h8000_0000_0000_0000, 1'b0);
 
    for (int i = 1; i <= 64; i = i + 1) begin
      check((64'h1 << i) - 1, 64'h1, 1'b0);
      check((64'h1 << i) - 1, 64'h0, 1'b1);
    end
 
    for (int i = 0; i < 64; i = i + 1)
      check(64'h1 << i, 64'h1 << i, 1'b0);
 
    $display("Total: %0d tests, %0d errors", tests, errors);

    if (errors == 0) $display("PASS");
    else             $display("FAIL");
 
    $finish;
  end
 
endmodule
