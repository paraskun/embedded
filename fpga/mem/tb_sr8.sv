`timescale 1ns / 1ps

module tb_sr8();

  logic       clk;
  logic       load;
  logic       sin;
  logic [7:0] d;
  logic [7:0] q;
  logic [7:0] expected;
  integer     errors;

  sr8 DUT(.clk(clk), .load(load), .sin(sin), .d(d), .q(q));

  always #10 clk = ~clk;

  task check_q(input logic [7:0] value);
    begin
      #1;
      if (q !== value) begin
        $error("q=%b expected=%b", q, value);
        errors = errors + 1;
      end
    end
  endtask

  task load_and_check(input logic [7:0] value);
    begin
      @(negedge clk);
      load = 1;
      d = value;
      expected = value;
      @(posedge clk);
      check_q(expected);
    end
  endtask

  task shift_and_check(input logic value);
    begin
      @(negedge clk);
      load = 0;
      sin = value;
      expected = {expected[6:0], value};
      @(posedge clk);
      check_q(expected);
    end
  endtask

  initial begin
    $dumpfile("out/sr8/sim.vcd");
    $dumpvars(0, tb_sr8);

    clk = 0;
    load = 1;
    sin = 0;
    d = 0;
    expected = 0;
    errors = 0;

    @(posedge clk);
    check_q(8'h00);

    load_and_check(8'ha5);
    shift_and_check(1'b1);
    shift_and_check(1'b0);
    shift_and_check(1'b1);
    shift_and_check(1'b1);
    shift_and_check(1'b0);
    shift_and_check(1'b0);
    shift_and_check(1'b1);
    shift_and_check(1'b1);

    load_and_check(8'h00);
    repeat (8)
      shift_and_check(1'b1);

    load_and_check(8'hff);
    repeat (8)
      shift_and_check(1'b0);

    if (errors == 0)
      $display("PASS");
    else
      $fatal(1, "FAIL: %0d errors", errors);

    $finish;
  end

endmodule
