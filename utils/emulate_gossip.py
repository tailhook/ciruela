import pprint
import random
from functools import partial
from collections import defaultdict


class Host:

    def __init__(self, id):
        self.id = id
        self.received = set()

    def gossip(self, packet, next_hosts):
        if packet not in self.received:
            self.received.add(packet)
            return next_hosts(self)


def random_gossip(host):
    return random.sample(HOSTS, 4)


def next_pair_gossip(host):
    idx = HOSTS.index(host)
    return [
        HOSTS[(idx+1) % len(HOSTS)],
        HOSTS[(idx+2) % len(HOSTS)],
    ]


HOSTS = list(map(Host, range(100)))
PKT_ID = 0


def emulator(func):
    global PKT_ID
    result = defaultdict(partial(defaultdict, int))
    for _ in range(1000):
        PKT_ID += 1
        start_host = random.choice(HOSTS)
        next_hosts = set(start_host.gossip(PKT_ID, func) or ())
        for iter_num in range(10000):
            buf = set()
            for h in next_hosts:
                buf.update(h.gossip(PKT_ID, func) or ())
            next_hosts = buf
            if not next_hosts:
                break
        n = sum(PKT_ID in h.received for h in HOSTS)
        result[iter_num][n] += 1
    return result

def print_totals(result):
    total_100 = 0
    total_val = 0
    for iter_num, values in sorted(res.items()):
        perc = values[100] / sum(values.values())
        total_100 += values[100]
        total_val += sum(values.values())
        print("Iterations {:2d}: {:6.2%}".format(iter_num, perc))
    print("Overall:       {:6.2%}".format(total_100 / total_val))


if __name__ == '__main__':
    print("Random gossip")
    res = emulator(random_gossip)
    print_totals(res)

    print("Next pair")
    res = emulator(next_pair_gossip)
    print_totals(res)

