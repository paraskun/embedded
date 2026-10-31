`timescale 1ns / 1ps

module tb_cnt5();

    logic       clk;
    logic       res;
    logic [2:0] q;

    cnt5 DUT(.clk(clk), .res(res), .q(q));

    always begin
        clk = 1'b1; #5;
        clk = 1'b0; #5;
    end

    initial begin
        $dumpfile("out/cnt5/sim.vcd");
        $dumpvars(0, tb_cnt5);

        res = 1'b1; #5;
        res = 1'b0; #5;

        #50;

        $finish;
    end

endmodule