`timescale 1ns / 1ps

module tb_mux4();

  logic [3:0] a;
  logic [1:0] s;
  logic       f;
  
  mux4 DUT(.a(a), .s(s), .f(f));
  
  initial begin
    $dumpfile("out/mux4/sim.vcd");
    $dumpvars(0, tb_mux4);

    s = 0; a = 4'b0001;             #10;
    repeat (3) begin
      s = s + 1; a = a << 1;        #10;
    end

    s = 0; a = 4'b1110;             #10;
    repeat (3) begin
      s = s + 1; a = (a << 1) + 1;  #10;
    end
      
  $finish;
  end

endmodule
