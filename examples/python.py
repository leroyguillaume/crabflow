import asyncio
from crabflow import PyLoopOrchestrator, PyTask, PyTaskGroup

async def main():
    a = PyTask(lambda : print("a"))
    b = PyTask(lambda : print("b"))
    c = PyTask(lambda : print("c"))
    group = PyTaskGroup([a, b], PyTaskGroup([c]))
    orch = PyLoopOrchestrator()
    status = await orch.process(group)
    print(status)

asyncio.run(main())
