`timescale 1ns / 1ps

module tb_sum64x3();

  logic [63:0] a;
  logic [63:0] b;
  logic [63:0] c;
  logic        cin;
  logic [63:0] s;
  logic  [1:0] cout;

  sum64x3 DUT(.a(a), .b(b), .c(c), .cin(cin), .s(s), .cout(cout));

  localparam logic [63:0] ONES = 64'hFFFF_FFFF_FFFF_FFFF;

  integer      errors = 0;
  integer      tests  = 0;
  logic [65:0] expected;

  task automatic check(input logic [63:0] ta, input logic [63:0] tb,
                       input logic [63:0] tc, input logic tcin);
    begin
      a   = ta;
      b   = tb;
      c   = tc;
      cin = tcin;

      #1;

      expected = {2'b0, ta} + {2'b0, tb} + {2'b0, tc} + tcin;
      tests = tests + 1;

      if ({cout, s} !== expected) begin
        errors = errors + 1;

        $display("FAIL: a=%h b=%h c=%h cin=%b -> got %b_%h, expected %b_%h",
                 ta, tb, tc, tcin, cout, s, expected[65:64], expected[63:0]);
      end
    end
  endtask

  initial begin
    // $dumpfile("out/sum64x3/sim.vcd");
    // $dumpvars(0, tb_sum64x3);

    check(64'h0, 64'h0, 64'h0, 1'b0);
    check(64'h0, 64'h0, 64'h0, 1'b1);

    check(ONES, 64'h1, 64'h0, 1'b0);
    check(ONES, 64'h0, 64'h1, 1'b0);
    check(ONES, 64'h0, 64'h0, 1'b1);
    check(ONES, ONES,  64'h1, 1'b0);
    check(ONES, ONES,  ONES,  1'b0);
    check(ONES, ONES,  ONES,  1'b1);

    check(64'h8000_0000_0000_0000, 64'h8000_0000_0000_0000, 64'h0, 1'b0);
    check(64'h8000_0000_0000_0000, 64'h8000_0000_0000_0000, 64'h5, 1'b1);

    check(64'h5555_5555_5555_5555, 64'hAAAA_AAAA_AAAA_AAAA, 64'h1, 1'b0);
    check(64'h5555_5555_5555_5555, 64'h5555_5555_5555_5555, 64'h5555_5555_5555_5555, 1'b1);

    for (int i = 1; i <= 64; i = i + 1) begin
      check((64'h1 << i) - 1, 64'h1,           64'h0, 1'b0);
      check((64'h1 << i) - 1, 64'h0,           64'h1, 1'b0);
      check(64'h0,            (64'h1 << i) - 1, 64'h0, 1'b1);
      check(64'h1,            64'h0, (64'h1 << i) - 1, 1'b0);
    end

    $display("Total: %0d tests, %0d errors", tests, errors);

    if (errors == 0) $display("PASS");
    else             $display("FAIL");

    $finish;
  end

endmodule
