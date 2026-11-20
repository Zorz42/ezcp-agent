Dana sta seznama $a_1, a_2, \dots, a_n$ in $b_1, b_2, \dots, b_m$ ter celo število $k$.

Iz njiju sestavimo seznam $c$ dolžine $n \cdot m$, ki vsebuje vsote $a_i + b_j$ za vse pare indeksov $i$ in $j$, kjer je $1 \le i \le n$ in $1 \le j \le m$. Vsak par $(i, j)$ prispeva svojo vsoto posebej, tudi kadar je ta enaka vsoti kakšnega drugega para.

Izpišite $k$ najmanjših vsot v seznamu $c$. Če se ista vrednost med njimi pojavi večkrat, jo izpišite tolikokrat, kolikorkrat se pojavi.

## Vhodni podatki

V prvih treh vrsticah so cela števila $n$, $m$ in $k$, vsako v svoji vrstici.

V naslednjih $n$ vrsticah so cela števila $a_1, a_2, \dots, a_n$, vsako v svoji vrstici.

V naslednjih $m$ vrsticah so cela števila $b_1, b_2, \dots, b_m$, vsako v svoji vrstici.

Velja $1 \le n \le 100\,000$, $1 \le m \le 100\,000$, $1 \le k \le \min(n \cdot m,\ 100\,000)$ in $1 \le a_i, b_j \le 10^9$.

## Izhodni podatki

Izpišite $k$ najmanjših vsot, urejenih naraščajoče, vsako v svoji vrstici.

{{SAMPLES}}
