import uuid
from datetime import datetime

from langchain.agents import create_agent
from langchain.tools import tool
from langgraph.checkpoint.memory import InMemorySaver
from dotenv import load_dotenv

load_dotenv()


@tool
def get_current_time() -> str:
    """Get the current local date and time."""
    return datetime.now().isoformat(timespec="seconds")


@tool
def add(a: float, b: float) -> float:
    """Add two numbers."""
    return a + b


agent = create_agent(
    "openai:gpt-4o-mini",
    tools=[get_current_time, add],
    system_prompt="You are a helpful assistant. Use your tools when they help.",
    # keeps each session's message history, keyed by thread_id
    checkpointer=InMemorySaver(),
)


def new_session():
    session_id = str(uuid.uuid4())
    print(f"[session {session_id}]")
    return session_id


session_id = new_session()
print("Type a message, /new for a new session, /quit to exit.")

while True:
    try:
        user_input = input("> ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        break

    if not user_input:
        continue
    if user_input == "/quit":
        break
    if user_input == "/new":
        session_id = new_session()
        continue

    # thread_id groups every turn of a session together, in the agent's memory and in the traces
    result = agent.invoke(
        {"messages": [{"role": "user", "content": user_input}]},
        config={
            "configurable": {"thread_id": session_id},
            "metadata": {"thread_id": session_id},
        },
    )
    print(result["messages"][-1].content)
