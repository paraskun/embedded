`timescale 1ns / 1ps

module tb_mux16();

  logic [15:0] a;
  logic  [3:0] s;
  logic        f;
  
  mux16 DUT(.a(a), .s(s), .f(f));
  
  initial begin
    $dumpfile("out/mux16/sim.vcd");
    $dumpvars(0, tb_mux16);

    s = 0; a = 16'b0000000000000001; #10;
    repeat (15) begin
      s = s + 1; a = a << 1;         #10;
    end

    s = 0; a = 16'b1111111111111110; #10;
    repeat (15) begin
      s = s + 1; a = (a << 1) + 1;   #10;
    end
      
  $finish;
  end

endmodule
