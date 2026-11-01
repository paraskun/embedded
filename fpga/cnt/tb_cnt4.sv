`timescale 1ns / 1ps

module tb_cnt4();

    logic       clk;
    logic       res;
    logic       ud;
    logic [3:0] q;

    cnt4 DUT(.clk(clk), .res(res), .ud(ud), .q(q));

    always begin
        clk = 1'b1; #5;
        clk = 1'b0; #5;
    end

    initial begin
        $dumpfile("out/cnt4/sim.vcd");
        $dumpvars(0, tb_cnt4);

        res = 1'b1; #5;
        res = 1'b0; #5;

        res = 1'b0; ud = 1'b1; #50;
        res = 1'b1;            #10;
        res = 1'b0; ud = 1'b0; #20;
        res = 1'b1;            #10;

        $finish;
    end

endmodule