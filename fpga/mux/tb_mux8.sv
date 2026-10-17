`timescale 1ns / 1ps

module tb_mux8();

  logic [7:0] a;
  logic [2:0] s;
  logic       f;
  
  mux8 DUT(.a(a), .s(s), .f(f));
  
  initial begin
    $dumpfile("out/mux8/sim.vcd");
    $dumpvars(0, tb_mux8);

    s = 0; a = 8'b00000001;         #10;
    repeat (7) begin
      s = s + 1; a = a << 1;        #10;
    end

    s = 0; a = 8'b11111110;         #10;
    repeat (7) begin
      s = s + 1; a = (a << 1) + 1;  #10;
    end
      
  $finish;
  end

endmodule
