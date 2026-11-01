`timescale 1ns / 1ps

module tb_cnt();

    logic       clk;
    logic       res;
    logic [3:0] q;

    cnt DUT(.clk(clk), .res(res), .q(q));

    always begin
        clk = 1'b1; #5;
        clk = 1'b0; #5;
    end

    initial begin
        $dumpfile("out/cnt/sim.vcd");
        $dumpvars(0, tb_cnt);

        res = 1'b1; #5;
        res = 1'b0; #5;

        #50;

        $finish;
    end

endmodule